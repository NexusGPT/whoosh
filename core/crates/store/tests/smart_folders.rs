//! real-folders: the smart folders of the store (`delta12.rs`: `smart_folders`, `smart_folder_save`, `smart_folder_rename`,
//! `smart_folder_update`, `smart_folder_delete`, `smart_folder_members`). Each test names its one-line broken copy (red first).
//! The speed test prints one line per operation and fails over 50 ms (docs/CONSTANTS.md INTERACTION_COMMIT_MS); run it with `-- --nocapture`, release profile.
mod common;
use std::time::Instant;

use common::*;
use contracts::CoreError;
use store::Store;

fn invalid(field: &str, words: &str) -> CoreError { CoreError::Invalid { field: field.into(), reason_words: words.into() } }
fn not_found(id: &str) -> CoreError { CoreError::NotFound { kind: "smart folder".into(), id: id.into() } }
fn names(s: &Store) -> Vec<String> { s.smart_folders().unwrap().into_iter().map(|f| f.name).collect() }

/// Broken copy: `smart_folder_save` binds `rule` where it binds `name` (the read back shows the rule as the name), or `tree_order` returns its rows as given.
#[test]
fn a_saved_folder_is_listed_in_tree_order_with_its_rule_and_survives_a_restart() {
    let db = TempDb::new("sf-order");
    let s = small_store(&db, 2);
    let a = s.smart_folder_save("  Hotels ", " from:@lorvan-hotels.example  booking ", 1).unwrap().unwrap();
    let b = s.smart_folder_save("Taxes", "tax", 2).unwrap().unwrap();
    let c = s.smart_folder_save("Invoices", "invoice", 3).unwrap().unwrap();
    assert!(a != b && b != c, "each folder has its own id");
    // saved order is the owner's order while all sit at the top level
    assert_eq!(names(&s), ["Hotels", "Taxes", "Invoices"]);
    let first = s.smart_folder(&a).unwrap().unwrap();
    assert_eq!((first.name.as_str(), first.rule_words.as_str(), first.parent_id.clone()), ("Hotels", "from:@lorvan-hotels.example  booking", None), "trimmed, the domain filter kept as typed");
    // Invoices moves inside Hotels, Taxes inside Invoices: parents come before children, whatever the saved order
    s.smart_folder_update(&c, "invoice", Some(&a), 4).unwrap().unwrap();
    s.smart_folder_update(&b, "tax", Some(&c), 5).unwrap().unwrap();
    assert_eq!(names(&s), ["Hotels", "Invoices", "Taxes"]);
    // a restart: the store is closed and opened again from the file
    drop(s);
    let s = Store::open(&db.0).unwrap();
    let again = s.smart_folders().unwrap();
    assert_eq!(again.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Hotels", "Invoices", "Taxes"]);
    assert_eq!(again[1].parent_id.as_deref(), Some(a.as_str()));
    assert_eq!(again[2].parent_id.as_deref(), Some(c.as_str()));
    assert_eq!(again[0].rule_words, "from:@lorvan-hotels.example  booking");
}

/// Broken copy: delete `if folders.iter().any(|f| f.name.to_lowercase() == lower)` in `smart_folder_save` (the duplicate is saved), or the `NeedsRule` check.
#[test]
fn a_save_is_refused_in_words_for_no_name_no_rule_or_a_name_in_use() {
    let db = TempDb::new("sf-save-refused");
    let s = small_store(&db, 2);
    s.smart_folder_save("Hotels", "booking", 1).unwrap().unwrap();
    assert_eq!(s.smart_folder_save("   ", "booking", 2).unwrap().unwrap_err(), invalid("name", "a smart folder needs a name"));
    assert_eq!(s.smart_folder_save("Taxes", "  ", 2).unwrap().unwrap_err(), invalid("rule", "a smart folder needs a rule"));
    assert_eq!(s.smart_folder_save("hOTELS", "other", 2).unwrap().unwrap_err(), invalid("name", "the name exists already"), "any case");
    assert_eq!(names(&s), ["Hotels"], "a refused save stores nothing");
}

/// Broken copy: `smart_folder_rename` compares every folder, its own too (renaming "Hotels" to "HOTELS" is refused), or skips the unknown-id check.
#[test]
fn a_rename_keeps_the_rule_and_is_refused_for_unknown_empty_or_taken() {
    let db = TempDb::new("sf-rename");
    let s = small_store(&db, 2);
    let a = s.smart_folder_save("Hotels", "booking", 1).unwrap().unwrap();
    s.smart_folder_save("Taxes", "tax", 2).unwrap().unwrap();
    s.smart_folder_rename(&a, "Stays", 3).unwrap().unwrap();
    s.smart_folder_rename(&a, "STAYS", 4).unwrap().unwrap(); // its own name in another case is allowed
    assert_eq!(names(&s), ["STAYS", "Taxes"]);
    assert_eq!(s.smart_folder(&a).unwrap().unwrap().rule_words, "booking", "the rule is not touched");
    assert_eq!(s.smart_folder_rename("sf-none", "X", 5).unwrap().unwrap_err(), not_found("sf-none"));
    assert_eq!(s.smart_folder_rename(&a, "  ", 5).unwrap().unwrap_err(), invalid("name", "a smart folder needs a name"));
    assert_eq!(s.smart_folder_rename(&a, "taxes", 5).unwrap().unwrap_err(), invalid("name", "there is already a smart folder named \"taxes\""));
    assert_eq!(names(&s), ["STAYS", "Taxes"]);
}

/// Broken copy: delete the walk up from the new parent in `smart_folder_update` (a loop is stored, the list then no longer shows a root), or the `ParentNotFound` line.
#[test]
fn an_update_changes_rule_and_parent_and_refuses_self_unknown_and_loops() {
    let db = TempDb::new("sf-update");
    let s = small_store(&db, 2);
    let a = s.smart_folder_save("A", "one", 1).unwrap().unwrap();
    let b = s.smart_folder_save("B", "two", 2).unwrap().unwrap();
    s.smart_folder_update(&b, "two from:@x.example", Some(&a), 3).unwrap().unwrap();
    let got = s.smart_folder(&b).unwrap().unwrap();
    assert_eq!((got.rule_words.as_str(), got.parent_id.as_deref()), ("two from:@x.example", Some(a.as_str())));
    assert_eq!(s.smart_folder_update(&a, "one", Some(&a), 4).unwrap().unwrap_err(), invalid("parent", "a folder cannot sit inside itself"));
    assert_eq!(s.smart_folder_update(&a, "one", Some(&b), 4).unwrap().unwrap_err(), invalid("parent", "a folder cannot sit inside one of its own folders"));
    assert_eq!(s.smart_folder_update(&a, "one", Some("sf-none"), 4).unwrap().unwrap_err(), not_found("sf-none"));
    assert_eq!(s.smart_folder_update("sf-none", "one", None, 4).unwrap().unwrap_err(), not_found("sf-none"));
    assert_eq!(s.smart_folder_update(&a, "  ", None, 4).unwrap().unwrap_err(), invalid("rule", "a smart folder needs a rule"));
    s.smart_folder_update(&b, "two", None, 5).unwrap().unwrap(); // None = the top level again
    assert_eq!(s.smart_folder(&b).unwrap().unwrap().parent_id, None);
    assert_eq!(s.smart_folder(&a).unwrap().unwrap().rule_words, "one", "a refused update changed nothing");
}

/// Broken copy: `smart_folder_delete` runs only the DELETE (the database's ON DELETE SET NULL then sends the children to the top level, not to the grandparent).
#[test]
fn a_delete_moves_the_folders_inside_it_up_to_its_parent() {
    let db = TempDb::new("sf-delete");
    let s = small_store(&db, 2);
    let top = s.smart_folder_save("Top", "t", 1).unwrap().unwrap();
    let mid = s.smart_folder_save("Mid", "m", 2).unwrap().unwrap();
    let leaf = s.smart_folder_save("Leaf", "l", 3).unwrap().unwrap();
    s.smart_folder_update(&mid, "m", Some(&top), 4).unwrap().unwrap();
    s.smart_folder_update(&leaf, "l", Some(&mid), 5).unwrap().unwrap();
    s.smart_folder_delete(&mid).unwrap().unwrap();
    assert_eq!(names(&s), ["Top", "Leaf"]);
    assert_eq!(s.smart_folder(&leaf).unwrap().unwrap().parent_id.as_deref(), Some(top.as_str()), "the child sits in the grandparent now");
    assert!(s.smart_folder(&mid).unwrap().is_none());
    assert_eq!(s.smart_folder_delete(&mid).unwrap().unwrap_err(), not_found(&mid), "a second delete finds nothing");
    s.smart_folder_delete(&top).unwrap().unwrap();
    assert_eq!(s.smart_folder(&leaf).unwrap().unwrap().parent_id, None, "a top-level parent: the child goes to the top level");
    // no mail was touched
    assert_eq!(s.rows_for_messages(&["m000".to_string()]).unwrap().len(), 1);
}

/// Broken copy: drop `AND lower(m.from_address) NOT IN (SELECT lower(address) FROM account)` (m005 is listed), drop `crate::read::VISIBLE`
/// (m002, m003, m004 are listed), or `ORDER BY m.received_at_ms ASC` (the order test fails).
#[test]
fn a_folders_members_are_the_inbox_mail_of_its_hits_newest_first_with_the_unread_mark() {
    let db = TempDb::new("sf-members");
    let s = small_store(&db, 10);
    s.write(|c| {
        c.tx.execute_batch(
            "UPDATE message SET archived = 1 WHERE id = 'm002';
             UPDATE message SET muted = 1 WHERE id = 'm003';
             UPDATE message SET folder = 'Sent' WHERE id = 'm004';
             UPDATE message SET from_address = 'owner0@example.test' WHERE id = 'm005';
             UPDATE message SET read = 1 WHERE id = 'm006';
             INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) VALUES ('m007', 9999999999999, 1);",
        )?;
        Ok(())
    })
    .unwrap();
    // the hits arrive in score order, with a duplicate-free list that names a gone id too
    let hits: Vec<String> = ["m000", "m009", "m002", "m003", "m004", "m005", "m006", "m007", "gone", "m001", "m008"].iter().map(|s| s.to_string()).collect();
    let got = s.smart_folder_members(&hits).unwrap();
    assert_eq!(
        got,
        vec![("m009".to_string(), true), ("m008".to_string(), true), ("m006".to_string(), false), ("m001".to_string(), true), ("m000".to_string(), true)],
        "archived, muted, sent, own-address, snoozed and gone left out; newest first; m006 is read"
    );
    assert!(s.smart_folder_members(&[]).unwrap().is_empty());
}

/// 2,000 stored mails, 50 folders: every write and the members read of 1,000 hits answer under 50 ms. Prints one line each.
/// Broken copy: a `std::thread::sleep(std::time::Duration::from_millis(60));` at the head of `smart_folder_save`.
#[test]
fn the_folder_operations_answer_under_50_ms_on_2000_mails() {
    const BUDGET_MS: u128 = 50;
    let db = TempDb::new("sf-speed");
    let s = small_store(&db, 2_000);
    let timed = |what: &str, run: &mut dyn FnMut()| {
        let t = Instant::now();
        run();
        let ms = t.elapsed().as_millis();
        println!("smart folders, {what}: {ms} ms on 2000 mails");
        assert!(ms < BUDGET_MS, "{what} took {ms} ms, over {BUDGET_MS} ms");
    };
    let mut ids = Vec::new();
    for i in 0..50 {
        let mut made = None;
        timed(&format!("save {i}"), &mut || made = Some(s.smart_folder_save(&format!("Folder {i}"), "from:@ext.test", 1).unwrap().unwrap()));
        ids.push(made.unwrap());
    }
    timed("rename", &mut || s.smart_folder_rename(&ids[10], "Renamed", 2).unwrap().unwrap());
    timed("update", &mut || s.smart_folder_update(&ids[11], "from:@ext.test tax", Some(&ids[10]), 3).unwrap().unwrap());
    timed("list 50 folders", &mut || assert_eq!(s.smart_folders().unwrap().len(), 50));
    let hits: Vec<String> = (0..1_000).map(|i| format!("m{i:03}")).collect();
    timed("members of 1000 hits", &mut || assert_eq!(s.smart_folder_members(&hits).unwrap().len(), 1_000));
    timed("delete", &mut || s.smart_folder_delete(&ids[10]).unwrap().unwrap());
}
