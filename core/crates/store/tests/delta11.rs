//! contracts-d11 hook: a label the owner removed from a thread is never filed back there by a rule or the model (0006).
mod common;
use common::*;
use contracts::*;

#[test]
fn a_removed_label_is_not_refiled_by_the_model_but_the_owner_can_put_it_back() {
    let db = TempDb::new("d11-removed");
    let s = small_store(&db, 4);
    let l = s.label_ensure_path("Clients".into(), vec![], 1).unwrap().label.id;
    s.label_file(l.clone(), "m000".into(), DecidedBy::Laya, Some(0.9), 2).unwrap();
    assert_eq!(s.label_remove_by_user(l.clone(), vec!["m000".into()]).unwrap(), vec!["m000".to_string()]);
    assert!(s.label_removed_by_user(l.clone(), "m000".into()).unwrap());
    assert!(s.label_removed_by_user(l.clone(), "m001".into()).unwrap(), "the removal holds for the whole thread (m000 and m001 share t000)");
    s.label_file(l.clone(), "m001".into(), DecidedBy::Jev, None, 3).unwrap();
    s.label_file(l.clone(), "m000".into(), DecidedBy::Rule { rule_id: "r".into() }, None, 3).unwrap();
    let carried = |id: &'static str| { let l2 = l.clone(); s.write(move |c| Ok(c.tx.query_row("SELECT count(*) FROM label_assignment WHERE label_id = ?1 AND message_id = ?2", rusqlite::params![l2, id], |r| r.get::<_, i64>(0))?)).unwrap() };
    assert_eq!((carried("m000"), carried("m001")), (0, 0), "neither the rule nor the model files it back");
    assert_eq!(carried("m002"), 0);
    s.label_file(l.clone(), "m002".into(), DecidedBy::Laya, None, 4).unwrap();
    assert_eq!(carried("m002"), 1, "another thread is not affected");
    s.label_file(l.clone(), "m000".into(), DecidedBy::Owner, None, 5).unwrap();
    assert_eq!(carried("m000"), 1, "the owner can put it back");
    assert!(!s.label_removed_by_user(l, "m001".into()).unwrap(), "which lifts the removal for the thread");
}

#[test]
fn remove_all_labels_records_each_removal() {
    let db = TempDb::new("d11-removed-all");
    let s = small_store(&db, 2);
    let l = s.label_ensure_path("Suppliers".into(), vec![], 1).unwrap().label.id;
    s.label_assign(l.clone(), vec!["m000".into()], 2).unwrap();
    s.labels_remove_all(vec!["m000".into()]).unwrap();
    assert!(s.label_removed_by_user(l, "m000".into()).unwrap());
}

#[test]
fn attachment_candidates_are_the_messages_whose_files_hold_every_word() {
    let db = TempDb::new("d11-candidates");
    let s = small_store(&db, 3);
    s.write(|c| {
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path, extracted_text) VALUES ('a1','m000','warehouse-photo.jpg','image/jpeg',1,'/x','pallets and a Forklift in the aisle')", [])?;
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('a2','m002','scan-notes.pdf','application/pdf',1)", [])?;
        Ok(())
    }).unwrap();
    assert_eq!(s.attachment_candidates("forklift", 10).unwrap(), vec!["m000".to_string()]);
    assert_eq!(s.attachment_candidates("scan notes", 10).unwrap(), vec!["m002".to_string()]);
    assert!(s.attachment_candidates("forklift notes", 10).unwrap().is_empty(), "every word, in one attachment");
    assert!(s.attachment_candidates("", 10).unwrap().is_empty());
}
